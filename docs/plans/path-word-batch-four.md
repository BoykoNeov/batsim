# The unit that was left out on purpose, and the two lessons it was holding

`docs/plans/path-article-shape.md` closed with a boundary rather than a blocker. Its reader
had just learned to see a quantity spelled as an article — *"an hour"*, *"a tenth of a
volt"* — and it deliberately did **not** admit `volt` to `UNIT_NOUNS`, because admitting a
noun forces every article phrase in that noun at once and two of the three sat on steps that
slice was not otherwise touching. It wrote both of them down, with their arms, so this slice
would not have to find them again.

**Status: LANDED 2026-09-01.** One unit noun, two new arms, ten vocabulary rules, three
claims given instant tags, and the last two lessons that were held off the word scan by a
volt are on it. `spelled` 21 → 23 steps of 32, `word_blind` 11 → 9. No engine code, no
snapshot bump, no wasm rebuild. Every number below was measured.

## What one word forced, and what it merely allowed

The distinction is the whole shape of the slice and it was settled before any code was
written, by enumerating the ban's own list. `BANNED_UNITS` is a superset of `UNIT_NOUNS` and
the ban runs on every lesson, so `[[english]]` in `web/path-claims.toml` is the complete
inventory of what admitting a noun can reach. Three of its forty-five phrases name a volt:

| phrase | step | in `spelled` already? |
| --- | --- | --- |
| *a tenth of a volt* | `and-it-is-still-in-there` | **yes** |
| *a full volt* | `the-electrolyte-starves` | no |
| *a volt* | `past-empty` | no |

So the **mandatory** cost of the word is one rule, on a step nobody was otherwise editing.
The other two are elective, and taking them is what closes the two lessons. That split was
confirmed rather than assumed: adding `("volt", 1.0)` alone and running the suite named
exactly one unaccounted phrase and no fourth one.

`millivolt` and `amp` stay out for the same reason `volt` did. *"about a tenth of a
millivolt"* on `what-it-cost` and *"down to hundredths of an amp"* on `nothing-to-clamp` are
both on steps in `spelled`, so admitting either noun is a rule due the same afternoon.
Neither is free and neither is this slice's.

## The two arms

Ten rules and eight of them are shapes the table already had. Two are not.

### `Tie::Provenance` — the one arm that is not about this simulation

Step 20 says its floor is a declaration and then says what a real cell does instead:

> the floor is `floor_v = 0.0`, a declared limit rather than a measurement — **a real
> reversed cell goes on to a volt or two negative** while its copper current collector
> dissolves.

No trajectory can answer that. No field holds it. The sentence is true, and the only place
this repo records it is the provenance note two lines above the value in
`chemistries/lfp_26650_generic.toml`: *"A real reversed cell continues to roughly -1 to -2 V
on copper dissolution before failing"*. The prose is a restatement of a comment in a file the
same sentence names.

Three routes were considered in this order and the third was taken:

* **Let a claim spell it.** Refused by the facts: `spells` requires a claim, a claim requires
  an engine measurement, and no run this engine can perform is about a real cell.
* **Declare it unclaimable and leave step 20 word-blind.** Honest, cheap, and it would have
  left six buildable arms unbuilt for one phrase. This was the recommendation to beat and the
  slice was scoped against it explicitly rather than drifting past it.
* **Read the note.** `Tie::Provenance { field, nth }` takes the run of comment lines
  immediately above a named key and reads the `nth` number out of it. `nth` is the fence: a
  provenance paragraph has enough digits in it that "the note contains this number somewhere"
  would find one by accident, which is the search-the-file match the whole taxonomy refuses.
  Here the note states an interval and the prose spells its near end, so the rule names
  number zero and wraps it in `Tie::Magnitude`, because the sign is in the word *negative*
  rather than in a character.

**Why a comment and not a field.** Every other chemistry arm reads a parsed value. The
alternative here was to invent a field for a number the engine never uses — a schema change,
a snapshot question and a validation rule for one sentence — and it would make a number the
simulation does not use look like one it does. A provenance note is where `CLAUDE.md` already
requires an unencoded physical fact to live; the gap was only that nothing could read one.
`docs/plans/hysteresis-width-over-soc.md` recorded the same gap from the other side, with its
cited magnitudes "held by provenance prose alone", so this is not a one-sentence arm by
design even though it has one user today.

### `Tie::Seconds` — and the reason both directions have to exist

`Tie::Hours` has been in the table since step 8 and it is **unusable for a quantity spelled
in words**. That is not a bug in it; it is the token carrying its own unit.

* A number in **digits** carries no unit into the scan. *"about 56 hours"* is the token `56`
  with a scale of one, so a tie reading a file in seconds must be divided down. That is
  `Tie::Hours`.
* A number spelled as a **word** carries its unit in the word. *"an hour"* is the token `1`
  with a scale of 3600, and the scanner does that conversion itself — so a tie feeding it
  must answer in **seconds**, and wrapping one in `Tie::Hours` divides by 3600 twice.

Which leaves the case step 16 states: *"set the current to 5.153198 A — the current that
would empty this cell in an hour"*. An amp-hour capacity over an ampere is a time in hours by
construction. The honest arm is that division, the scan needs it in seconds, and nothing
between them could say so.

**The alternative existed and is worse, which is why it is written into the variant's own
docs.** `Ratio(capacity_ah, Hours(demand))` reaches the same 3600 out of variants that
already exist, by reading an ampere "in hours" — a unit statement that means nothing, that
describes itself as such in the error text, and that arrives at the right number for no
reason the sentence gives. A variant is cheaper than a green that has to be explained.

## The two hours on step 16 are not the same arm, and that is the step's argument

The lesson says the current would empty the cell *in an hour*, and two sentences later that
*a full discharge is about an hour of simulation*. Both spell one hour; they are different
claims and only one of them is a measurement.

* The first is the **definition** of the hour rate: `cell.capacity_ah` over the current the
  reader is told to type, which is 3600 s exactly.
* The second is the arm's **measured** crossing, 3484 s, which is 0.97 of an hour — and
  *"about"* is the sentence being honest that it is not one.

Tying the definition to the measurement would be green today and green the day they parted,
and the whole point of the step is that on this model they part: it empties **before** the
hour, which is what the following sentence's `3484 s` against the twin's `3496 s` is about.

## The harness that could not put the tree back

The first run of the perturbation table died on its second case with `OSError: [Errno 22]`
writing the test file, and its `finally` restore hit the same error — so a harness whose
whole safety story is "every case is restored" left the tree in a state it could not
guarantee. It was intact, checked immediately and confirmed by a green suite, but that was
luck rather than design: a `"w"` open truncates before it writes.

Two occurrences of this hazard are already recorded here, both from a killed harness leaving
an edit behind (`docs/plans/path-ledger-what-it-cost.md`, `docs/plans/path-article-shape.md`). This is a
third shape of it — the restore itself failing — and the fix generalises: **a write is not
done until it has been read back**. The harness now verifies and retries every write and
raises rather than continuing, on top of the control-first gate the earlier occurrences
bought.

## What the checks found that reading did not

### One of the seven rules was not needed, and the check said so

Step 20's *"0.062 V a minute in"* was written as a rule — the claim's instant less the mark
the charge leg starts at, 4460 − 4400. It was refused as a **double accounting**, and the
refusal is right: check 6 already accounts that number. A claim read past the step's mark is
offered `read_at_s − until_s` as an instant, which is the same subtraction one layer up, and
the sentence's *"a minute"* had been invisible only because the step was never word-scanned.

The rule came out. **A phrase becoming readable does not always mean a rule** — sometimes it
means check 6 could already have answered and had never been asked. Nothing in the enumerated
plan predicted this; the double-cover panic is what found it.

### Tagging three readings blunted a `should_panic` test, exactly as that test predicted

`a_quotation_of_a_quantity_two_claims_disagree_on_is_refused` needs a real pair of claims
that share a quantity name and disagree. Its case was step 20's `v_at`, and its own docs said:
*"if some future slice tags step 20's readings too, this test stops having a case and says so
through the assertion below rather than passing on nothing."*

Reading *"thirty seconds later"* and *"at sixty"* off their instants meant telling those two
claims apart, which is a tag each — and the guard fired. A registered prediction confirmed by
the thing it predicted. The case moved to the same step's **charge leg**, which files five
voltages under one name and is quoted by nothing.

Worth noting what that guard is made of: its message deliberately avoids the phrase
`should_panic` is waiting for, because an earlier version's guard text satisfied the
`expected =` match and the test passed while proving nothing.

### A number the prose reads and the ban does not list

Step 20's charge-leg list runs *"0.025 V on the first step, 0.062 V a minute in, 0.064 V at
two"*. The third item is the same bare-numeral shape as *"at sixty"* two paragraphs earlier,
and it is **not read** — the shape requires its numeral to close on a comma, a full stop or
an *"and"*, and this one is followed by an em dash. So the scanner declines it, the ban does
not list it, and the step's count is seven rather than eight. That is written into the
`spelled` entry rather than left for a green to imply.

## The perturbation table

Sixteen cases against a control confirmed green first, each restored in a `finally`. Every
case's FAILED test names were read rather than its exit code, because the accounting check
panics in prose order and a table of red exit codes understates itself.

| case | verdict | what reddened |
| --- | --- | --- |
| control | GREEN | — |
| B — `volt` taken back out of `UNIT_NOUNS` | RED | three checks, including both word-list tallies and `every_ledger_rule_is_a_phrase_and_is_used` |
| C — step 24's tenth, second operand moved to another instant | RED | the accounting |
| **C2 — step 24's tenth pointed at the chemistry's `cell.v_min`** | **GREEN** | — |
| D — step 16's volt, difference order reversed | RED | the accounting |
| **D2 — step 16's twin read at 460 s instead of 464** | **GREEN** | — |
| E — the hour rate read in hours (`Tie::Hours`) instead of seconds | RED | the accounting, and the arm tally |
| **F — the hour rate read off `the twin at one c` instead of `one c`** | **GREEN** | — |
| G — the run length read on the step's own 3 C run | RED | the accounting |
| **G2 — the run length read on the twin's arm** | **GREEN** | — |
| H — *"thirty seconds"* pointed at the sixty-second reading | RED | the accounting |
| I — the provenance note's second number instead of its first | RED | the accounting |
| J — the chemistry's provenance note rewritten to −3 to −4 V | RED | the accounting |
| K — the magnitude wrapper dropped from the provenance arm | RED | the accounting |
| L — the whole provenance rule deleted, block and all | RED | the accounting, and the arm tally |
| M — the minute given a rule of its own again | RED | the accounting, on the double-cover panic |
| N — step 20 put back on the word-blind list | RED | six checks |

**Two cases were broken rather than evidence on the first pass, and both are recorded
because the failure modes are ones this repo keeps meeting.** D2's anchor matched twice — a
neighbouring rule quotes the same twin reading — and a case that edits two places is not the
case it says it is. L cut a rule's body out from between its own braces and bought a compile
error instead of a verdict, which is `docs/plans/path-untouched-steps.md`'s lesson exactly: a
deletion perturbation must remove the whole block. Both were re-run against a fresh green
control with the anchors fixed, and the verdicts above are the second pass.

**Four greens, and all four were predicted in the rule comments before the table ran.** That
is what they are for: each names a wrong arm that holds very nearly the right number, and
each is written into the rule it threatens rather than left in a plan doc.

* **C2** is the sharpest. Step 24's *"most of a tenth of a volt"* is 1.848076 − 1.749968 =
  0.098108. Point the second operand at the chemistry's own `cell.v_min`, which is 1.75 for
  this lead-acid cell, and it answers 0.098076 — the same `0.1`, off a threshold instead of
  off a reading. The rule is tied to the two claims because what the sentence contrasts is
  two READINGS, and the cut-off is a number the leg happens to end near; the arm pointed at
  the file would go on passing the day the leg stops somewhere else.
* **D2** says the same about the twin's instant: read four seconds earlier the gap is 1.0169
  instead of 1.0148, and both print as `1`. What holds *"at that same instant"* is the twin's
  own claim sitting at the instant the sentence names, not this arm.
* **F** and **G2** are the two arms of step 16 read off the sibling arm instead. Both arms
  set the same 5.153198 A and both crossings round to the same hour, so neither can be told
  apart by comparison. Each rule says which arm its sentence is about and why.

**The arm tally is a second objection on two of the cases, and that was not designed.**
`n_ledger_arms` counts the arm KINDS the rules actually use, so an arm with exactly one user
is watched by the file's own self-count as well as by its rule: deleting the provenance rule
(L) and taking `Tie::Seconds` out of its only rule (E) both move a number stated in words in
two files.

## Two silent-failure modes in the new machinery, asked about after it shipped

Neither is a defect in what landed — the suite was honestly green and the ten rules are
right — but both are places where the new arms could have failed quietly, and a table of
perturbations on the RULES cannot reach either.

**The empty resolution is already refused, and that was measured rather than assumed.**
`Tie::Provenance` resolves to nothing when the field path is wrong, the key has no note above
it, or `nth` is past the end — and `tie_agrees` compares with `values.iter().all(...)`, which
is `true` for an empty list. That would make a typo'd path pass silently, on an arm whose
whole job is reading a hand-typed path. It does not: `ledger_numbers` asserts
`!found.is_empty()` before it ever compares, with a message naming the rule and what it
reads. Case: `nth: 0` → `nth: 9`, which reddens the accounting.

**The arm-count walker did not descend into `Tie::Seconds`, and the count was right anyway.**
`n_ledger_arms` names the kind at the top of `walk` and then recurses only into the wrappers
its match lists. `Seconds` was not one of them, and everything inside this slice's only use
of it — a `Ratio` of a `Chemistry` and an `OnArm` of a `Setting` — is some other rule's
outermost tie, so 28 came out correct while the walker never looked inside. That is the
hazard the function's own doc names, in as many words, about `Tie::Hours`. `Tie::PerSecond`
had been in the same position since it was built.

Both are in the recursion now. Adding them moves no count today, which is the same
"unobservable but written the correct way round" note the walker already carries for
`Tie::Magnitude` — and the reason to write it that way is exactly this slice: the day a
wrapper holds the only use of what it wraps, a walker that stopped would undercount in
silence.

## The counts that moved

`spelled` 21 → 23 steps, and the two arrivals bring 3 and 7 spelled quantities; the tally
over the whole list is 20 → 31 quantities on 8 nonzero steps. `word_blind` 11 → 9.
`[[english]]` is unchanged at 45 phrases, because a phrase that gets a rule stays on the list
— it is still spelled in letters. The ledger's arm count is 26 → 28, stated in words in two
files and derived in both.

## Still unheld

Nine word-blind steps. The largest remaining are `pack-disagrees` (3, all about a spread
across the grid, which nothing in this file measures) and `three-times-the-current` (3, two of
which look closable and one of which is a per-step cost ratio between two cell models that no
file in this tree holds). The counts of engine steps and of cells on `two-legs`,
`what-protection-costs`, `lying-sensor` and `protection-on` still need an arm or a unit noun
before their zeroes mean anything.

Inside `spelled`, two phrases remain tied to nothing and both are units this reader does not
admit: `what-it-cost`'s *"a tenth of a millivolt"* and `nothing-to-clamp`'s *"an amp"*. The
third of that family is admitted now, and the two arms it needed were sitting in the previous
slice's plan doc — which is the argument for writing them down there.
