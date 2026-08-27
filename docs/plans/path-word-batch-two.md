# The second batch of word-scanned steps

The ledger reads quantities spelled in English as well as in digits, and until this slice it
did so for **one lesson of twenty-four**. The machinery was built and perturbed on
`the-gradient-itself`; everything else in the path was word-blind. This is the batch that
turns it on for six more, and the interesting part is not the six — it is what the survey
that chose them found, and what opening the scan on a lesson with different numbers in it
did to two checks that had been green the whole time.

## The survey came first

`ledger_numbers` gates the word scan on a per-step list, so the cheapest whole-path
measurement is a temporary test that ignores the gate and turns the accounting check's two
panics into a printed list. One run, every lesson, print-and-continue.

**68 quantities spelled in English across the path**, 15 of them on the step already
covered. Of the remaining 53, seven lessons reported none at all.

The survey also refused six phrases outright rather than reading them:
*"four and a quarter hours"*, *"two and a half days"*, *"four and a half seconds"*,
*"eight and a half seconds"*, and *"three and a half minutes"* twice. That is
`no_spelled_quantity_is_silently_skipped` doing exactly the job it was written for — its own
docs named that shape as the one it was pointing at — and it is the reason the batch begins
with a fourth scanner shape rather than with a list of steps.

## Zero is not zero

Seven lessons reporting no spelled quantity is the number to distrust, because a quantity
whose unit noun is not in the closed list produces **nothing**, which looks identical to a
lesson that spells no numbers. Read by hand, none of the seven is word-clean:

* `the-electrolyte-starves` says *"an hour of simulation"* twice. The numeral is an
  **article**, so there is no number word to key on — and that is also why the silent-skip
  guard cannot see it: that guard walks numerals.
* `protection-on` says *"Eight cells in 4S2P"*, `lying-sensor` says *"four healthy groups"*.
  Counts a scenario field settles, and in scope by the same rule that admits a measure at
  all; simply not nouns the table carries.
* `two-legs` and `what-protection-costs` count **engine steps** — *"Seven steps of taking"*,
  *"eight steps"*. Settled by the run length over `dt`, so that one needs an arm before it
  needs a noun.

None of the three is built here. All three are written down in `word_blind`'s own comment,
with the sentences that prove them, because a limit stated is not a waiver and a limit
inferred from a zero is not stated at all.

**The article shape is deliberately not built.** It needs an overlap pass, a "not after
`of`" fence to keep *"a fraction of a point"* from scanning as one point, and an interaction
with the scale words — three fences, none of which the silent-skip guard could watch, in a
file whose last two scanner defects were both fences written against the cases their author
thought of. It also decided the batch: `past-empty` was dropped, because its reading list
*"0.062 V a minute in, 0.064 V at two"* is headed by an article and the item after it
inherits nothing either.

## What was built

**Shape 4** — `<numeral> and a <fraction> <unit>`. Shape 2 the other way round: English
writes the same quantity in both orders and the position of the numeral is the only
difference, so a scanner reading one and not the other is reading a word order rather than a
shape.

**An overlap skip**, without which shape 4 does not work — and the reason it does not is the
first of this slice's two findings.

**`percent` and `percentage`** as unit nouns, for *"against the twin's half a percent"*.

**`word_blind`**, the counterpart list, with `every_lesson_is_word_scanned_or_named_as_not`
requiring every lesson to be in exactly one of the two. `spelled` alone says what is
covered; only a partition says what is not. At one step in twenty-four the omission spoke
for itself. At seven it stops speaking, and a lesson written next month would be word-blind
by default with nothing ever saying so.

**`Tie::Seconds`**, the mirror of `Tie::Hours`: a capacity over a current is a number of
*hours*, and every spelled duration is normalised to seconds by the scanner, so the tie has
to be too.

**Thirteen vocabulary rules**, almost all of one shape the first batch never met: an instant
stated **relative to another one**. *"eight and a half seconds after it"*, *"thirty seconds
later"*, *"ten seconds after the flag"*. None is a number any file holds; each is the
difference between two instants two claims are read at. `Tie::Instant` and `Tie::Difference`
both already existed — what was missing was an **address at both ends**, which is why eleven
readings on `nothing-to-clamp` grew instant tags in the same slice. Without them
`Tie::Instant` sees eight readings under one name and refuses, which is that fence working.

## The two coincidences

Both were live greens, both were found by opening the scan on a lesson whose numbers
happened to collide, and both are the same defect one layer apart.

**1. The scanner handed a wrong arm the right number.** Before shape 4 existed,
*"40.33 A four and a half seconds later"* was read by shape 1 as the two words `half
seconds` — 0.5 s. `nothing-to-clamp` holds its step length at exactly 0.5 s. So the ledger
tied a sentence stating **4.5** to a control holding **0.5** and reported green. The phrase
the scanner could not read was not skipped; its *tail* was read instead, and the tail was
accountable.

**2. Check 6 compared a percentage against a duration.** Adding `percent` immediately
produced the same collision one level up: *"half a percent"* is 0.5, and the two arms of
`accounting_without_arithmetic` that compare **numbers** — the step length, and the instant a
claim is read at — are both in seconds and are offered every spelled quantity, because a
number needs no digits to be compared. `Written::scale` cannot separate them: a second, a
point, a percent and a multiplier all scale by one. So `Written` carries the **unit noun**
now, and those two arms are gated on it being a length of time. The gate only ever narrows;
a number written in digits carries its unit in the prose around it and is untouched.

After the gate, `claim setting` does not appear anywhere in a whole-path scan. That arm's
only spelled match, in the entire path, had been the wrong one.

## What a claim `spells` is now read for words too

`nothing-to-clamp` carries a claim whose `spells` is the word `"fifty"` and whose value is
50.3811, written for *"this fault costs **fifty points**"*. Check 5 holds the measurement to
it. The ledger nevertheless reported that sentence as tied to nothing, because the arms that
compare *characters* are refused to spelled quantities — a rule written against `"24"`
matching a digit `24` somewhere else in the group.

The refusal was too wide. `spells` can itself be a **word**, and a word compared to a word
has no formatting coincidence available: `"fifty"` matches `fifty` and nothing else. Under
the blanket refusal the repair on offer would have been a vocabulary rule re-deriving a
number the claim beside it already measures, which is the duplication this whole file is
arranged against.

## Three numbers that moved

Each is a sentence the scan could reach for the first time, and each was wrong.

| was | is | why |
| --- | --- | --- |
| *"about two and a half days"* | *"about 56 hours"* | 200 000 s is **2.31** days. No fraction the scanner can spell rounds to that at the precision it would commit to, and a `Setting` is compared exactly, so no unit conversion of the mark could be checked in days at all. Hours land on a whole number. |
| *"against the twin's half a percent"* | *"against the twin's 0.56 points"* | the twin lost **0.55719** points, which rounds to 0.6 at the one place *"half"* commits to. Points on both sides also makes the contrast a subtraction the reader can do. |
| *"Seven of the sixteen spelled quantities"* | *"...of the fifteen"* | a self-count in the first batch's own header. The scan found fifteen before a line of this slice was written; it had never found sixteen. |

The second one is the one worth keeping. The claims file **already knew the number**: a note
beside that sentence read *"the twin's half a percent is step 18's 0.55719, which is claimed
there"*. A figure written down in prose, beside a claim, compared with the twin by nobody.
Two files agreeing in English while disagreeing in arithmetic is the shape this ledger keeps
finding, and it is never found by re-reading; it is found by giving a check the reach to ask.

## What a perturbation round established

Nine cases, each declaring the exact set of tests it expected to redden, and a harness that
**refuses to start unless the baseline is green** — the previous slice lost a whole round to
a killed foreground harness that captured its own mutated tree as the restore point.

| perturbation | reddened |
| --- | --- |
| *"eight and a half seconds"* → *"nine and a half"* in the prose | the ledger, alone |
| the overlap skip is deleted | the scanner's own test, **the ledger, and the word counts** |
| shape 4 stops reading its unit | the scanner test, the silent-skip guard, the ledger, **and the rule-usage guard** |
| `unit_is_time` is widened to admit a percent | the new fence, alone |
| a step is removed from `spelled` and not added to `word_blind` | the partition guard, **the file's own "seven" tally, and the rule-usage guard** |
| a `word_blind` count is wrong by one | the derived-count guard, alone |
| the quoted twin figure moved by a hundredth | the ledger, alone |
| the twin rule is pointed at the wrong lesson | the ledger, alone |
| CONTROL: prose reworded, no number touched | nothing |

Three cases reddened **more** than predicted and none reddened less, which is the direction
that costs nothing. The two worth reading are the ones where a check I had not thought of
caught the change first: deleting the overlap skip moves the word *counts*, because the
tail becomes a second quantity; and disabling shape 4 leaves the two `… later` rules
matching nothing at all, which the rule-usage guard says out loud.

**The fence case is the one that had to be built rather than found.** Repairing the prose
removed the only sentence in the path that reaches the unit gate, so the gate would have
been pinned and consulted by nothing — the shape this file rejects everywhere else. It has
a test of its own now, on `an_hours_tie_rounds_the_way_a_computed_tie_does`'s terms: the
same token, the same claims, the same lesson, and the only difference is the noun.

## Round two, and the wire nothing was watching

The table above was measured **before** `accounting_for`'s four number parameters were
bundled into a `Reading`, and green-after-a-refactor does not re-establish that a fence still
*bites*. Re-running it turned up a hole the first round could not have seen.

`Written::unit` reaches the gate through `From<&Written>`, and **nothing exercised that
conversion**. The fence test hand-builds its `Reading` to ask the predicate a clean question;
the scanner's own test compared tokens and scales and dropped the noun; and every sentence
that once collided has been repaired. `unit_is_time("")` is **true** — empty means "the
sentence wrote digits", which those arms have always been asked about — so a conversion
passing `""` would have re-opened the collision with the whole suite green.

| perturbation | reddened |
| --- | --- |
| `From<&Written>` drops the unit noun | **the new wire test, and nothing else** |
| shape 4 stops setting the unit noun | the wire test and the scanner's own |
| `unit_is_time` is widened to admit a percent | the fence, and the wire test |
| the overlap skip is deleted | the scanner test, the ledger, the word counts, the wire test |
| CONTROL: prose reworded, no number touched | nothing |

**The first row is the finding.** One test reddens, and it is the one written in this round —
which is the demonstration that nothing else was watching, rather than an argument that
nothing was. The general shape is worth naming: *the predicate had a test and the plumbing
into it did not*, which is the same "pinned and consulted by nothing" defect one level over
from the one the gate itself exists to fence. A fence is only as live as the wire feeding it,
and a wire is only watched if a perturbation on it reddens something.

The scanner's own test now returns the noun beside the token and the scale, and asserts it on
every case — including the list-ellipsis item, which is the one place the noun is *copied*
from a neighbour rather than looked up, and a non-duration noun, which is the case the gate
is actually about. Both `percent` and `seconds` scale by one, so the scale can never stand in
for the noun.

## One thing the arms did to the code

`accounting_for` reached eight parameters and clippy refused it. The four that describe the
number — its characters, its unit noun, that unit's scale in seconds, and whether the
sentence spelled it in letters — are now a `Reading`, which is the honest shape: they are
one thing, three of the four are consulted by a *different* arm, and the fourth arrived the
day one of those arms turned out to be answering about a quantity that was not a duration.

## Deferred, with a price

* **Seventeen lessons are still word-blind**, named in `word_blind` with what a scan finds
  in each. The largest is `what-it-cost` at eight quantities.
* **Three shapes are unread**, listed above with the sentences that prove them: the article,
  the topology count, the count of engine steps. The first is the one that blocks a step
  (`past-empty`), the second is cheap the day a scanned step needs it, the third needs an
  arm.
* **`pack-disagrees` wants claims that do not exist.** Its three quantities are the spread
  across the grid at the mark — *"about half a point"*, *"a quarter of a point"* between the
  two cells of a pair — and nothing measures a spread. That is a claim-side slice, not a
  word-side one.
* **The unit gate is narrow on purpose.** It knows "is this a length of time" and nothing
  else, because that is the only distinction the two gated arms need. A third arm reading,
  say, amperes would need the table to grow, and the honest version of that is a unit *kind*
  per noun rather than a predicate per arm.
