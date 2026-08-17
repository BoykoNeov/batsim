# Four counts about these files were wrong, and all four were spelled in letters

`docs/plans/path-self-counts.md` made the two path-claims files describe themselves
honestly: a sentence declares the *phrase*, the check derives the *number*, and a count
that moves without its sentence moving fails by name. It fixed nine stale counts and left
the coverage opt-in, which means the hole it did not close is the sentence nobody opted in.

This slice swept for those. Four counts were stale, three of them within two lines of a
derived one, and **every one of the four is spelled in English** — which is why the earlier
pass could not see them: it reads digits.

| where | said | is |
| --- | --- | --- |
| the ledger's "not covered" entry | `fifteen` steps unreached | fourteen |
| the same entry, twice in one sentence | `nine` whole steps closed | ten |
| the same entry, naming `unledgered` | `all fifteen`, one line each | all fourteen |
| the opening paragraph | read the header for `the four checks` | that header lists six |

The last one is the oldest: it has been wrong since the fifth check landed, and it sits
eleven lines above a heading that says "The six checks".

## What was done about each

Three of them are counts of the ledger's own two lists, so they are **derived** now —
`n_unledgered` beside the `n_ledgered` that already existed, and three new `TALLIES`
entries. The fourth is not derivable (the number of checks is a property of the test code,
which is the argument the claims file's own waiver already makes), so that sentence was
**reworded to carry no number at all**. That is not a new move: the self-counts slice did
the same to the `grid` tally's twin, and it is the only fix that removes the rot site
rather than guarding it.

One more count was already correct and undeclared — check 6's "five accounting arms",
derived in the claims file's header and hand-written in this test's — and it is a tally
now. Left undeclared it was one slice away from being the next entry in the table above.

Four sentences whose counts *cannot* be derived are written into `NOT_DERIVED` rather than
left silent: the two remaining statements of "six checks", the "five above" that is that
count less one, this test's copy of "four slices found numbers", and the claims file's
"all seven are about a CLAIM". A waiver there is required to still match its sentence, so
each of them now fails if it is reworded — which is the whole difference between a written
gap and an unwritten one.

## Perturbations, registered before the run

| edit | must redden |
| --- | --- |
| prose `in the fourteen steps` → `in the thirteen steps` | the new unledgered tally |
| prose `but only ten` → `but only nine` | the new ledgered tally (second placeholder) |
| prose `names all fourteen` → `names all fifteen` | the new unledgered tally, second sentence |
| prose `check 6's five accounting arms` → `four accounting arms` | the new arm-count tally |
| move `slow-and-patient` from `[ledger].steps` to `unledgered` | all four count sentences at once — the direction that matters, a content move with the prose standing still |
| reword the heading `The six checks, and why none of them is redundant` | the new `NOT_DERIVED` entry for it |

All six reddened as registered, and the last one reddened on **one test more than it was
registered for**: moving `slow-and-patient` out of the ledger also fails
`every_ledger_rule_is_a_phrase_and_is_used`, because the four vocabulary rules that step is
the only user of then match nothing in any ledgered step. That is the "no rule goes unused"
fence doing exactly its job, and it is worth naming as a property rather than a surprise:
**the ledger's coverage list and its vocabulary are coupled in the fail-toward-red
direction.** A step cannot quietly leave the scan while the rules written for it stay behind
looking like coverage.

## While sweeping: what the next step actually costs

The stale advice above is not confined to counts. `docs/plans/path-ledger-dfn-step.md` closed
with a recommendation for which step to ledger next, and it was wrong in both halves. A
temporary instrument — the ledger's own `cover_by_rule` and `claimed_accounting` run over the
fourteen unledgered steps in print-and-continue mode — settles it. Per step: numerals in its
whole prose, claims it carries, and numerals nothing in the tree accounts for today.

| step | numerals | claims | unaccounted |
| --- | --- | --- | --- |
| `wearing-out-while-idle` | 16 | 5 | **8** |
| `two-legs` | 19 | 8 | **8** |
| `bare-curve` | 15 | 3 | 12 |
| `nothing-to-clamp` | 35 | 19 | 12 |
| `protection-off` | 24 | 9 | 15 |
| `looks-fine-from-outside` | 38 | 13 | 19 |
| `one-step-that-got-through` | 32 | 9 | 21 |
| `leg-that-is-not-there` | 28 | 6 | 22 |
| `what-protection-costs` | 28 | 3 | 25 |
| `past-empty` | 42 | 13 | 27 |
| `three-times-the-current` | 36 | 6 | 31 |
| `same-discharge-other-chemistry` | 40 | 5 | 35 |
| `what-it-cost` | 58 | 19 | 35 |
| `the-gradient-itself` | 42 | 4 | 38 |

`the-gradient-itself` was the recommended next step and it is the **last** on this list. The
reasoning behind the recommendation — "its numbers are almost all measurements on the pre-Run
probe" — is the reason it is expensive: a measurement needs a claim, a tolerance with a
derived rule, a frame, and an arm to be read on, where a constant needs one line of
vocabulary. The recommendation had the sign backwards.

**A ledger slice's cost is the unaccounted count, not the numeral count.** `what-it-cost` and
`the-gradient-itself` print about the same number of figures and differ by a factor of four
in what is left to do, because nineteen claims already stand behind one of them.

The next two steps are `wearing-out-while-idle` and `two-legs`, and each needs one capability
that does not exist yet — the first has a number only an *arm's* control setting decides (the
45 °C the reader drags to), the second a number only the page's own **source** decides (the
10 s the CC-CV rule is re-checked on, pinned in `MIRRORED` and read by `cccv_window_steps`).
Both are one arm each, which is the shape every ledger slice so far has had.

> **Both are done, and the prediction held on both** — `docs/plans/path-ledger-idle-step.md`
> and `docs/plans/path-ledger-two-legs-step.md`. `Tie::OnArm` and `Tie::Page` are the two
> capabilities, one each, named here before either existed. One detail above is wrong and
> the slice found it: `cccv_window_steps` did **not** read the pin, it carried its own copy
> of the `10`. The pin had no reader at all, which is the sharper version of the same point.

## Learned while building

**A count spelled in letters is invisible to a scanner that reads digits, and this file has
now been caught by that twice.** The self-counts slice fixed nine counts and recorded the
lesson as "two counts wore one label"; what it did not record is that its own instrument
could only see half the problem. Both slices' worth of stale counts were words. There is no
check that *finds* an undeclared count — `TALLIES` is opt-in and `NOT_DERIVED` is the written
gap — so the sweep is a periodic manual pass, and the honest statement is that it will be
needed again.

**Three of the four stale counts sat within two lines of a derived one.** That is the third
instance of a pattern this file has already written down twice — the `grid` pair and the
unledgered split — and at three instances it stops being a coincidence: a sentence gets
derived, its neighbours are read at the same time by the same author, and the neighbours are
what gets left. When adding a tally, the thing to check is not the sentence you are deriving
but the paragraph around it.

**The oldest defect was the one furthest from the work.** "Read its header for the four
checks" is in the opening paragraph, eleven lines above a heading that says "The six checks",
and it survived the slice that fixed nine counts elsewhere in the same doc comment. A sweep
that starts where the last slice finished will not find it; the one that reads the file top to
bottom does.

