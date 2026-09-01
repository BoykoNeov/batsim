# The third batch of word-scanned steps

The ledger reads quantities spelled in English as well as in digits, and it does so for
**fifteen lessons of thirty-two**. `docs/plans/path-word-numerals.md` built the scanner on
one step; `docs/plans/path-word-batch-two.md` opened it on six more. This is the batch that
opens it on the next four to six, and it is the first one where the scan has something to
read on the day it lands.

**Status: LANDED 2026-09-01.** Five steps moved, `spelled` 15 → 20 and `word_blind` 17 → 12,
thirteen spelled quantities accounted for by eleven new vocabulary rules and one widened.
The sixth candidate did not move and the last section says why. Everything below was
authored before the edits and is left as written except where a line says it was measured;
the two sections at the foot are the measurement.

## The one fact that decides how to read a green here

**Every step now in `spelled` reads zero spelled quantities.** All fifteen entries carry
`# 0`. That is not an accident of which steps were chosen: `docs/plans/path-digits-rule.md`
landed *after* batch two, banned English quantities in lesson prose and rewrote thirty-five
phrases into digits — including every one on the steps batch two had just opened. So
`ledger_numbers`'s spelled branch contributes nothing anywhere in the path today, and batch
two's thirteen vocabulary rules for spelled quantities have no live user.

This batch is therefore the first live exercise of the English accounting path since the
digits rule. **A green on a newly scanned step is unproven until a data perturbation on one
of its spelled phrases reddens something that had to compute first.** That is the standing
rule of this file and it applies with extra force here, because the alternative reading of a
green — "the scan is wired up and reads nothing" — is exactly what was true five minutes
before the batch started.

## The survey did not have to be rebuilt

`every_count_beside_a_word_list_entry_is_derived` asserts the count in each `word_blind`
comment against `spelled_numbers` of that step's prose, so batch two's survey is permanently
encoded in `web/path-claims.toml` and is current. What still had to be run was an
**enumeration** — which phrases, not how many — and the recipe is batch two's: turn the
ledger's accounting panic into a `println!` and continue.

**Measured 2026-09-01, sixteen gaps across the six candidate steps**, and nothing else red:
check 6 (`every_number_in_a_claimed_literal_is_accounted_for`) and the silent-skip guard both
pass untouched with all six scanned. So the whole of this batch's work is vocabulary rules.

| step | phrase | what should decide it |
| ---- | ------ | --------------------- |
| `belief-drifts` | *over ten minutes* | the run length, `until_s = 600` |
| `protection-off` | *ten seconds later* | 345.0 − 335.0, two claimed instants |
| `looks-fine-from-outside` | *the eighteen minutes* | the continuation's 1060 s, rounded |
| `looks-fine-from-outside` | *three times its rated hour rate* | demand over the cell's capacity |
| `particle-remembers` | *the ten minutes are up* | the pulse train's off-leg, 600 s |
| `particle-remembers` | *Ten minutes of rest* | the same off-leg |
| `particle-remembers` | *sixty seconds of diffusion* | the on-leg, 60 s |
| `particle-remembers` | *the same five times* | how many teeth the run holds |
| `what-it-cost` | *three and a half minutes away* | the knee at 207.5 s, rounded |
| `what-it-cost` | *is three and a half minutes* | the same knee |
| `what-it-cost` | *two hundredths of a point* | 99.98 − 99.96, the control arm's whole point |
| `what-it-cost` | *two seconds later* | an instant no claim reads yet |
| `what-it-cost` | *a quarter-second of wall clock* | the client's own sampler period |
| `three-times-the-current` | *ten times the circuit's arithmetic* | **nothing in the tree** |
| `three-times-the-current` | *ten minutes earlier* | 11 880 − 11 280 |
| `three-times-the-current` | *its sixty seconds* | the on-leg, 60 s |

## Order, and why it is not by size

Cheapest first, so that stopping early still banks whole steps:

1. `belief-drifts` — one phrase, one control.
2. `protection-off` — one phrase, a difference of two instants the step already claims.
3. `looks-fine-from-outside` — two, one of which is a rounded reading of a claimed instant.
4. `particle-remembers` — four, three of them the pulse program.
5. `what-it-cost` — five, and the first with a phrase that may need a **new claim**.
6. `three-times-the-current` — three, and one of them may have no honest arm at all.

`and-it-is-still-in-there` has the most (six) and is **not** in this batch. All six are
hours and three are stated relative to the rest's start, which is the shape that cost batch
two eleven instant tags on a neighbouring step. It is a slice, not a batch member.

`past-empty` (five) stays out for batch two's reason: its reading list is headed by an
article, and the article shape is still unbuilt.

## The two phrases that may not close

**`what-it-cost`'s *"only reaches 0.1 % two seconds later"*.** The instant the debt clears is
claimed; the instant the row first prints `0.1 %` is not. If no claim reads there, the honest
close is a new claim rather than a rule, and a rule pointed at the nearer of two instants
would be the wrong-arm-right-number shape this file keeps finding.

**`three-times-the-current`'s *"about ten times the circuit's arithmetic per step"*.** No file
in the tree holds a cost ratio between two cell models, and the sentence itself says why an
absolute would be worse. If it cannot be tied, this step does **not** move to `spelled`: its
`word_blind` comment gets the sentence written into it, the way `pack-disagrees` and the
engine-step counts already are. A limit stated is not a waiver, and a step moved with a
phrase tied to the nearest number lying around is worse than a step left alone.

## What else has to move

* `no_spelled_quantity_is_silently_skipped`'s own doc says it "walks seven lessons" and that
  "Every step in `spelled` now spells nothing at all". The first is stale by eight
  (`spelled` holds fifteen) and the second stops being true the moment this batch lands.
  Both are prose, so no digit scanner sees them — the `path-self-description-sweep` shape.
* `HEADER_WORDS` will need whatever the two list lengths come to in words.
* The counts beside every moved entry, and the self-counts both files keep.


---

# What the edits came to

## Every proposed tie was right, which is not the usual result

All thirteen phrases closed on the first run of the ledger, with no tie disagreeing. That is
worth recording precisely because it is unusual in this file, and the reason is that the
survey ran **first**: the enumeration printed the phrase, the token, the unit and the scale
the scanner had already read, so each rule was written against a quantity whose value was
known rather than guessed. The step that cost the most was not a rule at all — it was
noticing that two claims had to become individually addressable before one of them could be
pointed at.

**Eleven new rules and one widened.** The widened one is the interesting entry. Step 15 had

```
phrase: "of this cell's {n}, in the eighteen minutes",
```

— a rule whose *literal text* contained a quantity, because when it was written nothing on
that step could see one. That is what a word-blind step looks like from inside the
vocabulary table: the phrase reaches straight past a number and treats it as scenery. It now
reads `"of this cell's {n}, in the {n}"` and the second slot is the continuation's cut-off
said in minutes.

**Two claims grew instant tags.** `what-it-cost` filed three readings of `soc_at` on its
charge leg, two at 983 s and one at 985 s, and the sentence *"only reaches 0.1 % two seconds
later"* is the gap between them. `Tie::Instant` refuses a quantity that answers at more than
one instant on one step and arm — correctly — so the 985 reading became `soc_at:985` and the
983 pair `soc_at:983`. Same shape as the eleven tags `nothing-to-clamp` grew in batch two,
same reason: an address is what makes a reading quotable.

## The perturbation table, and the two cases that measured nothing

Fourteen cases, each editing one thing, each running the whole binary and recording every
red test rather than an exit code. **Ten proved the arm under test actually computed**, and
the evidence is the failure message naming both sides: *"says `four times` where the lesson's
DemandValue control divided by the chemistry's `cell.capacity_ah` says [3.0]"* is a rule that
did the division.

Three of those ten are **data** perturbations, which is the half that matters given that this
vocabulary had never read a spelled token before:

* `until_s` 600 → 540 on step 4 — *"says `ten minutes` where the lesson's Until control says
  [540.0]"*.
* `CELLS_PERIOD_MS` 250 → 500 on the page — *"says `quarter-second` where the page's
  `CELLS_PERIOD_MS` constant says [500.0]"*, and the mirrored-constant guard fired beside it.
* the deletion case: putting one step back in `word_blind` reddens five tests, including the
  partition, the counts and two rules that then match nothing.

**Two cases reddened on the wrong claim, and neither was a result.** This is the failure mode
`path-ledger-bare-curve` recorded — read the log, not the exit code — and it recurs here in a
new shape: *an English numeral in the prose is text, so it can be load-bearing for something
other than the arm you are testing.*

* Changing *"the same five times"* to *"six times"* broke a **pre-existing rule's literal** —
  `"its {n} mV was the same five times"` — which then matched nothing, and the scan failed on
  an unaccounted `74.8` two clauses earlier. The tooth-count ratio was never reached.
* Changing *"two seconds later"* to *"four seconds later"* broke a **claim's own literal**, so
  the `0.1` beside it lost its accounting and failed first.

Both were re-run, and a third round was needed for the second of them. The tooth count fell
to a data perturbation — `until_s` 3300 → 2640 — which reports *"says `five times` where the
lesson's Until control divided by the lesson's PulseOn control plus the lesson's PulseOff
control says [4.0]"*: the division happened.

**The two-instant rule took two attempts, and the near miss is the more useful one.** Moving
the claim it addresses (`soc_at:985` → `soc_at:987`) reddens the rule, but the message says
the rule could not **resolve** its operand — not that the subtraction disagreed. That is a red
proving the wiring and nothing about the arithmetic, and it is structural rather than
careless: on a tagged reading the address **is** the instant, so no data edit can move the
value without also moving the name. What settles it is the **consistent** perturbation the
last slice named — the prose sentence and the claim literal quoting it moved together, which
is what an author who miscounted actually produces — and there the rule reports *"says `four
seconds` where the instant ... `soc_at:985` is read at ... less the instant ... `soc_at:983`
is read at ... says [2.0]"*.

The general lesson is narrow and worth keeping: **a prose perturbation on a spelled quantity
is not automatically isolated**, because the word is text that other rules and other claims
quote; and **a data perturbation on a tagged instant cannot be isolated at all**, because the
tag is the datum.

## The table understated its own coverage, and fail-fast is why

Two of the thirteen phrases were **never individually reddened**, and counting cases rather
than reading them is what hid it. `every_numeral_in_a_ledgered_step_is_accounted_for` panics
on the first disagreement *in prose order*, so a case that reddens an earlier phrase says
nothing whatever about a later one:

* `"{n} of rest does not undo {n} of diffusion"` — the on-leg slot was proved by case G. Its
  **rest-leg** slot was not: the case that moves `off_s` reddens
  `"returned when the {n} are up"`, which sits earlier in the same paragraph.
* `"of a point is {n} of perfectly ordinary discharge"` — never touched. The case written for
  *three and a half minutes* edited the **other** sentence that states the same instant.

This is `dfn-aging-gap-closed`'s lesson recurring *inside one binary* rather than across
binaries, where the answer was `--no-fail-fast`. Inside a binary there is no flag: the answer
is to walk the rule table against the case list by hand and check that every slot has a case
that reaches it. Both closed on the next run — *"says `Twenty minutes` where the lesson's
PulseOff control says [600.0]"* and *"says `four and a half minutes` where ... the claim on
`flag_first_s:SOC_CLAMPED_LOW` says [207.5]"*.

**And the deletion case was testing a rename.** Round one renamed the entry rather than
moving it, which fires "in neither list" and "no lesson" together — two artificial failures
that mask what a real regression looks like. The honest version puts the entry back in
`word_blind` with its count intact, and it reddens two things: this file's own self-count
tally, and `every_ledger_rule_is_a_phrase_and_is_used` on the five rules that then match
nothing. That second one is the useful half — it means the rules written here cannot be
orphaned in silence.

Sixteen cases in four rounds. Every one of the thirteen phrases now has a case whose failure
message shows both sides of a comparison that had to be computed first.

## The slice planted four new self-counts, in words, and had to take them back out

The claims file states counts about itself and
`every_count_these_files_state_about_themselves_is_derived` checks the ones registered in its
table. This batch wrote three new ones into that file's own header **without registering
them** — how many word-scanned entries read nothing, how many read something, and how many
quantities they carry — plus two more into a doc comment. The tally check stayed green
throughout, because it only looks at phrases the table names.

One of the three was worse than rot-prone. It read *"THE FIRST FIFTEEN ENTRIES ALL READ
ZERO"*, which is a claim about **where in the list an entry sits**, and the five steps that
read something were appended at the end. Nothing enforces that ordering: a re-sort would have
made the sentence false with every check in this file green.

All three are derived now, over the **whole** list rather than its head —
`n_spelled_steps_reading_none`, `n_spelled_steps_reading_some`, `n_spelled_quantities` — and
mistyping any of the three words reddens the tally with the sentence it should have said. The
two in the doc comment were reworded to carry no numeral, because nothing derives a
paragraph. This is `path-self-description-sweep`'s defect class, introduced and closed inside
one slice, which is the only way this file has ever avoided paying for it later.

## The step that did not move, and why that is the honest answer

`three-times-the-current` has three spelled quantities. Two would close today — the pulse
train's on-leg, and *"ten minutes earlier"*, which is the gap between two instants the step
already claims. The third is

> This costs about **ten times** the circuit's arithmetic per step at 20 shells on a 1S1P pack

and no file in this tree holds a cost ratio between two cell models. The sentence itself says
why an absolute would be worse: *"the microseconds move by half again between sessions on one
machine, so any absolute here would be a number about a laptop rather than about a model."*

Step 16 met exactly this shape and `every_numeral_in_a_ledgered_step_is_accounted_for`'s own
doc records what it did: **"Five of its numbers left the page instead: two per-step cost
ratios, which no trajectory settles."** Checked rather than assumed before this was written:
a grep across the tree finds the ratio discussed in six plan documents and in this test's own
module docs, and held by no file anything can read — `path-untouched-steps.md` states it
outright as *"Three perf ratios no trajectory can settle."* That precedent is available here and was not taken,
because taking it means deleting a true sentence a reader benefits from, and deleting a true
sentence to clear a scan is the defect `path-twin-arm` names. So the step stays word-blind
and its entry now carries the sentence, the way `pack-disagrees` carries the spread it cannot
measure. A limit written down is not a waiver; a limit inferred from a step's absence is not
written down at all.

## Still unheld

* **Twelve steps are word-blind**, named with what a scan finds in each. The largest is
  `past-empty` at five and `and-it-is-still-in-there` at six.
* **The article shape is still unbuilt**, and it is what blocks `past-empty`,
  `the-electrolyte-starves` and the *"a fraction of a point"* on step 4 that this batch
  scanned right past.
* **Counts of engine steps and counts of cells** (`two-legs`, `what-protection-costs`,
  `lying-sensor`, `protection-on`) each need an arm or a unit noun before their zeroes mean
  anything.
* `and-it-is-still-in-there` is the natural next slice and is a slice rather than a batch
  member: six quantities, all hours, three of them stated relative to the rest's start.

## Two things recorded rather than built

**The refused step's entry states a prediction, and says so.** Its comment notes that two of
its three phrases look closable — the pulse train's on-leg and a gap between two claimed
instants. Neither rule was written here, so that is a prediction and not a measurement, and
the entry is worded to say which. `phase-8-slice-c-spike.md` is why: a plan's confidently
named third ingredient turned out not to exist, and the registration is what showed it.

**The tooth count has a second reading this rule does not distinguish.** *"was the same five
times"* is tied to the mark over one tooth — 3300 over 660 — but it can equally be read as a
count of the rebounds observed, and the step next door files exactly five of them. The two
agree at five only because this mark lands on a whole tooth. A train whose mark fell mid-tooth
would leave the arm green and the sentence about something else, and the perturbation that
moves `until_s` cannot separate them because both readings move with it. The program is the
reading taken, because it is the one a reader can check on the page; the alternative is now in
the rule's own comment rather than left to be rediscovered.
