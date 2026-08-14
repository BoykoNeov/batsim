# The ledger, built for the three steps that needed no measurement

`docs/plans/path-prose-ledger.md` measured every number in the fourteen guided-path steps
that carry no claim, found two defects in about 145 measurement-shaped figures, and then
said the thing that mattered more than either defect: **nothing in that pass stops the next
drift.** Both defects had been introduced by slices that ran a fully green suite, and they
could not have reddened anything, because a step with no claim has no literal for any check
to scan.

This slice builds the first piece of the instrument that does redden. It is deliberately the
cheapest piece: the three steps whose every *numeral* is a constant some file already
declares, so they can be closed before a single quantity is measured.

## What landed

* **The ledger** — `every_numeral_in_a_ledgered_step_is_accounted_for` scans a step's *whole*
  prose, not the sentences a claim quotes, and requires every number in it to be tied to
  something. Opt-in per step through a new `[ledger]` table in `web/path-claims.toml`.
* **One arm, the scenario file** — a number is accounted if it is the value of a *named
  field* of the scenario the step loads, in the unit the prose writes it in.
* **The vocabulary that makes that arm mean anything** — `SCENARIO_VOCABULARY`, thirteen
  phrases mapping the way the prose says a thing to the key that decides it: `"{n} in
  series"` → `pack.series`, `"a +{n} mV offset"` → `faults.*.fault.SensorOffset.offset`.
* **A two-list contract** — `[ledger].steps` and `[ledger].unledgered` together must name
  every lesson exactly once.
* **A bound on the lesson scraper**, which turned out to be a live hole rather than tidiness.
  See below.

Coverage, stated the way this file states everything: **three steps, fourteen numerals, one
arm.** Twenty-one steps are named as not covered — ten of them have their claimed sentences
checked by the seven claim checks and the rest of their prose free; eleven have nothing at
all.

| step | numerals | what decides them |
| --- | --- | --- |
| `pack-disagrees` | 4 | `pack.series`, `pack.parallel`, both `pack.scatter` sigmas |
| `belief-drifts` | 3 | the BMS's current offset, its noise, its boot error |
| `lying-sensor` | 7 | the two `[[faults]]` tables, read straight off the file |

**A ledgered step is digits-closed, which is not the same as closed**, and the distinction
matters enough to be here rather than in the deferred list. The scanner finds numerals; a
quantity spelled in English is invisible to it, and these three steps state four that way:

* `pack-disagrees` — "about **half a point** across the whole grid by the end, of which **a
  quarter of a point** is between the two cells of a single pair". Both are engine
  measurements: the spread of true SOC across eight cells at the 300 s mark, and the part of
  it that is inside one parallel group.
* `belief-drifts` — "a gap of about **three points** that simply never closes", and the
  sensor offset adding "only **a fraction of a point** over ten minutes". Both are
  `soc_bms − soc_true`, which is the whole subject of the step.

Those four are the sentences a reader actually leans on, and nothing checks them. The check
is named `every_numeral_…` for that reason. Two of them are also the cheapest claims
available anywhere in the remaining eleven steps: the estimator gap needs no zero-length
probe and no instructed continuation, only a run to the mark and the `claimed` arm.

## The design question, and the answer that cost the most

The arm has to name the field. The generous version — "this number appears somewhere in the
step's scenario file" — accounts for about a third of the path's numbers and means nothing:
a scenario has enough integers in it that a `2` finds `parallel = 2` by accident, and the
`0` in "cell (1,0)" would be satisfied by either coordinate of either temperature probe. So
*something* has to say which field a sentence is about.

That something is the phrase around the number, and it is the only thing declared here. The
number itself never is: it is read out of the scenario file and compared, so a rule pointed
at the wrong field fails on sight — the same property `Accounted` was built with, and for
the same reason. Pointing `"{n} in series"` at `pack.parallel` produces

```
step `pack-disagrees` says `4` where scenarios/soft_short_under_a_lying_sensor.toml
says [2.0] at `pack.parallel`
```

Two fences keep the vocabulary from becoming the loophole:

* **A phrase has to carry words.** A bare `"{n} mV"` would match any millivolt figure in any
  ledgered step and account it against the sensor offset — silently correct until the day
  one of those figures happens to be 120. Rules are required to contain letters, and each
  `{n}` must have something on at least one side of it.
* **A rule that matches nothing fails.** This file has already been caught by the opposite:
  `CCCV_PERIOD_S` sat pinned in `MIRRORED` for six slices with nothing reading it, and the
  mirror it was meant to guard was wrong the whole time. A vocabulary entry left behind by a
  prose edit would read as coverage of a number that is now failing somewhere else.

### The wildcard was existential, and that was a hole

`*` in a path walks an array, so `faults.*.at_s` does not care what order the file lists its
two faults in — the alternative, a literal index, would turn a harmless reordering of the
file into a red test about the prose.

It was written as "*some* value at this path is the number", and that is a fail-toward-green
on the one relational thing `lying-sensor`'s prose says: the short and the sensor lie land
"**in the same instant**". Measured rather than argued — the second fault's `at_s` moved to
700 s, everything else untouched:

| arm | `every_numeral_in_a_ledgered_step_is_accounted_for` |
| --- | --- |
| wildcard existential (as first written) | **green**, with the sentence false |
| wildcard strict (this slice) | **red** |

So a wildcard is now read strictly: every value it reaches must be the number. For the other
five wildcarded paths that is the same check — each reaches exactly one value — and for
`at_s` it is the difference between "a fault happens at 600" and what the sentence says.

The cost is stated rather than left to be discovered: a *third* fault at some other time
would fail this rule even though the sentence, which names two, would still be true. That is
the fail-toward-red direction, and the answer then is a path that selects the fault the
sentence names — which this walker cannot express.

## Found on the way: the last lesson's block ran past the end of the lessons

`lessons()` splits `const LESSONS` on the next `id:` marker, so the **last** lesson's block
ran from its own `id` to the end of `web/app.js` — some 240 lines of `proseHtml`, `setWatch`
and the rest. `and-it-is-still-in-there` carries eight claims, and check 1 for all eight was
therefore a substring test against the page's source code as well as against its own prose.

Measured rather than argued, because "unlikely to matter" is exactly what a green check says
about itself. One of that step's claims was given a literal that appears only in the JS
*after* the array closes — `the backticks are the point` — and the two arms differ in one
character of the scraper:

| arm | `every_claim_appears_in_its_own_step` |
| --- | --- |
| array bounded at its `];` (this slice) | **red** |
| array unbounded (before this slice) | **green** |

So the hole was real and is now closed. No claim in the tree was actually relying on it —
all 67 still pass — which makes this a hardening rather than a repair, and it is the
prerequisite for ever ledgering that step: a whole-prose scan there would have been
inventorying the numbers in a function body.

The other arm of that pair is contaminated, and it is worth naming: changing a literal also
reddens `every_tolerance_follows_its_declared_rule`, which requires `spells` to be among the
literal's own numbers. The exit code says nothing in either arm; the per-test result is what
carries the finding. A harness that had only compared exit codes would have called both arms
red and concluded the bound changed nothing.

## What was measured, not assumed

Fifteen perturbation cases, launched at below-normal priority with real exit codes (a
`start /wait` wrapper hides the child's status — twice recorded in this repo), each recording
*which* test reddened rather than only that something did:

| perturbation | reddens |
| --- | --- |
| no perturbation at all — the null | nothing, exit 0 |
| prose says `5 in series` | ledger |
| prose says `30 mA high` | ledger |
| `capacity_sigma` moves in the scenario | ledger |
| prose gains an untied number (`17 kg`) | ledger |
| a vocabulary rule is deleted | ledger |
| a vocabulary rule matches nothing | rules |
| a rule points at the wrong field | ledger |
| a rule points at a field that is not there | ledger |
| the second fault stops being simultaneous | ledger — existentially green, see above |
| a lesson is in neither list | lists |
| a lesson is in both lists | lists (and the ledger, which now scans it) |
| a claimed step is ledgered | ledger, on the fence that refuses it |
| a JS-only literal, array bounded | literal |
| the same, array unbounded | **nothing** — the control arm above |

The null case is in the table on purpose. A harness that cannot report green has not
reported anything, and this one has been fooled before: `docs/plans/surface-vs-bulk.md`
records five green perturbations that were all lies.

## What was deliberately not built

Every one of these is a capability the next slices need. None of them has a consumer *today*,
and this file's own history is the argument for not landing them early — a pinned constant
nothing reads is the shape that hid a wrong CC-CV mirror for six slices.

* **The `claimed` arm.** A number a claim already ties to the engine should be accounted by
  check 6's `accounting_for`, which is written and tested. But no ledgered step has claims,
  so the arm would be untested code, and a ledgered step *with* claims would fail confusingly
  instead. So the test refuses to ledger a claimed step and says why. This is the first thing
  the next slice needs.
* **The `setting`, `chemistry`, `ordinal` and `derived` arms**, designed in
  `path-prose-ledger.md`. `derived` is still the one with no precedent: it checks a token
  against other tokens in the same sentence, which needs the sentence parsed as an
  expression, and the cheap version is a declaration.
* **The zero-length probe row.** Three steps quote a reading taken before the reader presses
  Run, and four more decompose a pulse with it. The harness samples only stepped rows, and
  that gap is one-sided: every affected number reads slightly low, which looks exactly like
  drift.
* **Instructed continuations and control changes.** A BMS toggle, two step lengths, an
  ambient change, `Clear queued`, `Clear latched BMS fault`, and "press Run again" — all
  reproduced by the temporary harness in the measuring slice, none of them by `run()`.

## Deferred, with a price

* **A quantity spelled in English is invisible, and four of them are in the ledgered steps.**
  Named above with what each one measures. The scanner would need a word vocabulary ("half a
  point", "about three points") and, behind it, the `claimed` arm and a measured claim — so
  this is next-slice work rather than a gap that can be closed by wording. Until then, the
  three green steps mean "every digit is tied to a file", not "this step is checked".
* **Fourteen numerals out of about 350 is not a coverage claim, and the eleven wholly
  unchecked steps are still wholly unchecked.** Step 19's six stale figures and step 14's
  false contrast would both still be invisible today; the repairs are in the prose, and
  nothing guards them. The list in `[ledger].unledgered` is the honest statement of that, one
  line per step naming what it is waiting for.
* **The three ledgered steps are the easy ones, and closing them proves the frame rather
  than the arms.** Every number in them is a file constant. The steps that carry
  measurements need the probe, the continuations, and a claim each — which is where the
  remaining three or four slices go.
* **A number a rule accounts for is checked against the scenario file, not against the
  engine.** `"{n} in series"` matching `pack.series = 4` says the sentence agrees with the
  file the page loads; it does not say the pack that gets built has four groups in series.
  That is the scenario loader's job and `crates/sim-data/tests/scenario.rs` has it, but the
  ledger is not evidence about the engine and should not be read as any.
* **The vocabulary is prose-shaped, so a reworded sentence fails as "nothing accounts for
  this number".** That is the fail-toward-red direction and the message says to add or fix a
  rule — but an author who rewords `"20 mA high"` will meet a failure about the number, not
  about their wording.
* **`unledgered` carries a one-line reason per step, and nothing checks those reasons.**
  They are the same kind of authoring note as a claim's `note`: useful, and not an assertion.
