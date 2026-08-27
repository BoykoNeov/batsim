# The digits rule

The guided path's lesson prose states its numbers in **digits**. A quantity spelled in
English — *"half an hour"*, *"about ten times"*, *"an hour of simulation"* — is refused by
`no_lesson_spells_a_quantity_in_english`, and the forty-eight left standing when the rule
landed are named one phrase at a time in `[[english]]` in `web/path-claims.toml`. Forty-eight
rather than the thirty-five the survey found, because the ban reads a shape and a set of units
the reader never did — five of them were in the prose all along and invisible to everything.

This is slice 0 of Phase 8, and it is a prerequisite rather than an improvement: slices B and
D write new lesson prose, and prose written before the rule is prose written in the old style.

## What this replaces

`docs/plans/path-word-numerals.md` and `path-word-batch-two.md` built a **reader** for the
quantities the prose spells in English. It reached **7 of 24 steps** across two slices, and
finishing it was seventeen more rounds of the same work — building a shape, then a fence for
the shape, then a vocabulary rule for each phrase the shape newly reached.

`docs/plans/phase-8-chemistries.md` records the owner's decision: **ban the practice instead
of reading it.** The digit scanner already covers all twenty-four steps; make "numeral" and
"quantity" mean the same thing in the prose it scans, and it becomes the whole of the
coverage.

**The argument is about which way the mistakes fall, and it is worth stating plainly because
neither instrument can be right every time.** Deciding whether an English number word is
acting as a quantity is the same hard problem in both directions — *"one of the cells"* is not
one and *"one volt"* is. What changes is the cost of being wrong. A false alarm under a ban is
one sentence rewritten by the author who just wrote it, with the failure naming the phrase. A
miss under a reader is silent, and a wrong number ships behind a green suite. This repo has
shipped that second shape and written it down more than once.

## The measurement that changed the slice's size

The plan document costed slice 0 as "one pass rewriting the quantities into digits". It is
not, and the number that says so was taken before a line was edited: a temporary harness ran
the word scan over **all** twenty-four steps with the per-step gate disabled and printed what
accounted for each quantity rather than panicking.

**70 quantities spelled in English. 35 of them accounted for by something. 35 by nothing.**

That second half is the finding. A quantity spelled in letters is invisible to the digit
ledger, and on a step the reader was never turned on for it is invisible to that too — so it
sits in the prose tied to nothing at all, which is exactly the state the ledger exists to end.
Rewriting one into digits **makes the ledger see it**, and the ledger has no waiver variant.
So the rewrite is not an edit; each of those 35 is a vocabulary rule, a claim, or a reworded
sentence.

**The ban saves the scanner work, not the arm work.** The seventeen rounds it retires are
rounds of building shapes and fences; the accountings were always the expensive half and they
arrive through the digit door instead of the word door. That distinction was not in the phase
plan and it is the reason this slice was scoped the way it was.

## What was built, and what was deferred

Put to the owner as three shapes with their costs. The choice was the middle one:

* **Rewrite the 35 that already had an arm**, re-point every rule and claim that read a word,
  and add the ban.
* **Name the other 35 phrase by phrase** in a list that is matched *both ways*, so it can only
  ever get shorter, and work it down in a later slice.
* **Leave the reader in place.** Nothing points at a word any more, so it now reads nothing —
  but taking it out is a cascade of its own and it is not this slice's job.

### The rewrite

Thirty-five prose edits, every literal that quotes them, twenty-three vocabulary-rule phrases,
three claims whose `spells` was a word, and two rules deleted because their sentences no longer
have a relative instant in them.

**Every non-second duration is a unit re-derivation, not a word-for-digit swap**, and this is
the trap the slice was designed around. A spelled quantity carries a `scale` — `Written::scale`
is 60 for a minute — so *"an eighteen-minute discharge"* is compared as 1080 s. A **digit**
carries no unit at all, so `"18-minute"` would be compared as 18 and quietly break. The
systematic answer, taken once and applied everywhere:

* a spelled quantity whose unit scales by one (seconds, points, times, percent) becomes the
  same number in digits, and every arm behind it is untouched;
* a duration in minutes becomes **seconds**, which is the unit the rest of the path already
  writes: *"at three minutes"* → `at 180 s`, *"twenty-four minutes"* → `1464 s`, *"half an
  hour"* → `1800 s`;
* a duration the arm itself holds in **hours** stays in hours and the arm loses its conversion:
  *"7.2 Ah if you take twenty hours over it"* → `20 hours`, and `Tie::Seconds` came off both
  rules that used it.

**`Tie::Seconds` is gone as a result, and that is the honest consequence rather than a
casualty.** It existed for one purpose — a spelled duration is normalised to seconds by the
scanner, so a tie answering in hours had to be converted — and the ban removes the only thing
that ever reached it. It was the mirror of `Tie::Hours`, which survives, because a *digit*
still says "about 56 hours" on step 11.

**Teaching `written_numbers` to read a trailing unit noun was considered and killed.** It looks
like the tidy fix — let a digit carry a scale the way a word does — and it would have broken
*"about 69 minutes"*, *"about 56 hours"*, *"11.5 seconds"* and *"53.5 seconds"*, every one of
which is tied today as a bare number.

### Three numbers moved out of a claimed sentence, and why they had to

Check 6 requires every number inside a claimed literal to be accounted for **by the claims on
that sentence** — a vocabulary rule is not available to it. Three of the rewritten quantities
were relative instants inside claimed sentences, tied on the *ledger* side only, and the moment
they became digits check 6 could see them and had nothing to answer with:

| was | is | why |
| --- | --- | --- |
| *"40.33 A four and a half seconds later"* | `40.33 A at 240 s` | its own reading list already says *"17.03 A at 250 s, 1.94 A at 300 s"* — the absolute instant is the sentence's own style, and it is the claim's `read_at_s` |
| *"the same 87.02 A thirty seconds later"* | `the same 87.02 A at 90.5 s` | same, and the instant it was relative to is not printed in the sentence at all |
| *"…at 245.5 s**, ten seconds after the flag"* | literal split in two | the offset is real and a rule ties it; splitting the two claims' literals leaves it outside both, where the ledger can reach it and check 6 need not |

The two rules that read the first two are deleted. The third — *"10 s after the flag"* — is the
last relative instant on that step and it kept its rule.

### The ban

`english_quantities` is a **detector**, and it is the opposite of `spelled_numbers` in the one
way that matters: it produces no value. A phrase it cannot parse is exactly as banned as one it
can, so the whole apparatus of shapes-and-fences that the reader needed simply does not apply.

Four shapes: numeral-then-unit, unit-then-`and a`-fraction, the hyphenated attributive, and —
the one the reader never had — **the quantity spelled as an ARTICLE**, *"an hour of
simulation"*, *"a minute in"*. That shape has no numeral word in it to key on, which is why
`no_spelled_quantity_is_silently_skipped` cannot see it either, and the reader's own plan doc
declined to build it because valuing one needs three fences none of which that guard could
watch. Refusing one needs none.

**`BANNED_UNITS` is deliberately wider than `UNIT_NOUNS`, and the reason is the same one.** The
reader's noun list is narrow because a reader that finds a quantity it cannot tie either fails
loudly or hands it to whatever arm happens to hold the right number — the collision
`path-word-batch-two.md` records twice. **A forbidder has no arm.** So the ban knows volts,
amps, ohms, watts, kelvin, degrees, milliseconds, weeks, months and years, not one of which is
spelled anywhere in the path today; they cost nothing now and they are what a new lesson would
reach for. Two guards hold the relationship: `the_ban_refuses_every_unit_the_reader_reads` over
the two tables, and `the_ban_sees_every_quantity_the_reader_reads` over the real prose, which
is the half a table comparison cannot reach.

**Furniture stays out of both, and now for a sharper reason than inheritance.** *"two
electrodes"*, *"Four footnotes"*, *"three decimals"* are a list's length rather than a measure,
and this repo has no arm that reads a list. Banning them would force a rewrite into digits that
the ledger would then see and be unable to account for — a worse place for the number than the
one the module header already declares it to be in.

### The fence that was built and then taken out again

The first draft refused the article shape after `of`, so that *"a fraction of a point"* would
not read as the number one. It bought three entries off the backlog list, and it cost a false
**negative** the same afternoon: *"empty is three and a half minutes away instead of an hour"*
states an hour and is not a partitive, and the fence swallowed it.

That is the shape this file has now built three times — a fence written against the cases its
author thought of, inheriting exactly the blind spot it was guarding against. It came out. A
partitive costs one entry on a list; a missed quantity costs a number nothing checks.

### The list

`[[english]]` is 48 phrases across 12 steps. It is matched **both ways**: a phrase in the prose
that is not listed fails, and a phrase listed that is not in the prose fails too — so repairing
a sentence means deleting its entry and the list can only get shorter. There is no way to add
to it that does not look like what it is.

It is **more** than the 35 the survey found, because the ban reads the article shape and the
wider noun list: *"a full volt higher"*, *"most of a tenth of a volt"*, *"hundredths of an
amp"*, *"a tenth of a millivolt"* and five articles were all invisible to the reader.

Three phrases were **reworded rather than listed**, because they are not quantities at all:
*"a second 3 C discharge"*, *"a second tooth"*, *"a second number beside it"* — ordinals, where
`second` is the unit noun by coincidence. That is the false-alarm cost the ban was accepted
for, and it came to three sentences.

## Findings

**1. The plan's own article count was wrong, and in the direction that flatters the plan.**
`phase-8-chemistries.md` recorded "roughly 30 article-form phrases, and treat it as a floor",
hand-counted from a grep over the whole of `web/app.js`. Scanned over **lesson prose only**,
which is all the ban is about, the article shape yields about seven quantities the reader could
not already see. The plan's grep was counting code comments and ordinals. The correction cuts
both ways and the honest statement is narrow: *in the prose*, the article form is small; the
grep's number was never about the prose.

**2. A count the machinery derives caught a rewrite the machinery could not.** `belief-drifts`
turned *"about three points"* into `about 3 points`, and the first thing to redden was
`every_count_beside_a_ledger_entry_is_derived` — that step's prose prints four numerals now and
its `[ledger]` comment said three. Eleven per-step counts moved the same way. The counts are
derived from a scan and the comments are not, which is precisely why they moved.

**3. Two words in `WORD_NUMERALS` died with the rewrite, and the guard said so by name.**
`three` and `fifty` were there for the three claims whose `spells` was a word. All three now
spell digits, and `every_word_numeral_is_read_by_something` refused the table entries the same
minute. What is left is `six`, read by a derivation's operand, and `zero`, read by an arm's
instruction — neither of which is a claim's own figure. **`spells` may still hold a word**; no
claim does.

**4. The `spelled` list now reads zero on every entry, and that is the result rather than a
gap.** Seven steps, twenty-nine quantities, all rewritten. The reader still runs over them and
finds nothing, which makes `no_spelled_quantity_is_silently_skipped` vacuous — said out loud in
its own doc comment, because a green over an empty list reads exactly like a green over a full
one.

### The guard the list needed, which it did not have when it was written

The per-step headers over each `[[english]]` block state how many phrases are under them, and
for one commit **nothing derived those numbers**. That is the defect class this file guards
against in four other places and has caught stale four separate times, introduced in the slice
whose subject is numbers nothing checks — the backlog's own tally counts the list *whole*, so
it says nothing about the distribution.

`every_count_above_an_english_block_is_derived` closes it, on
`every_count_beside_a_word_list_entry_is_derived`'s terms. **And perturbing it immediately
found it half-built.** Walking the steps the list holds misses the case that matters most: a
step whose *last* phrase is repaired leaves the list entirely, and its header then sits over a
block of nothing with nothing to compare against — which is precisely what finishing a step
looks like. Both directions now.

## Perturbations

Eleven cases, each declaring the exact set of tests it expected to redden, run by a harness
that refuses to start unless the baseline is green and restores the tree after every case.

| perturbation | reddened |
| --- | --- |
| a lesson gains a spelled quantity — *"and never trips for ten seconds"* | the ban, **and that step's word-list count** |
| an `[[english]]` entry is deleted | the ban, and the backlog tally |
| an `[[english]]` entry names a phrase the prose does not have | the ban, and the backlog tally |
| a unit the reader reads leaves `BANNED_UNITS` | the ban, and **both** superset guards |
| the article shape stops firing | the ban, and the article shape's own test |
| the list-ellipsis shape stops firing | the ban, and the prose-side superset guard |
| `1800 s` back to *"half an hour"*, prose alone | six: the ban, the ledger, the rule-usage guard, two derived counts, the self-count tally |
| `50 points` back to *"fifty points"*, prose **and** its literal | seven: the ban, check 6, the tolerance rule, the unit-gate fence test, two derived counts, the tally |
| a per-step `[[english]]` header count is wrong by one | the new count guard, alone |
| an `[[english]]` entry is deleted and its header left behind | the ban, the count guard, and the backlog tally |
| CONTROL: prose reworded, no number touched | nothing |

**No case rested on the ban alone, and the two that go back to words are the ones worth
reading.** Reverting one rewritten phrase reddens between six and seven checks, because the
rewrite is load-bearing in five places at once: the sentence, its literal, the rule that reads
it, the count of that step's numerals, and the tolerance the claim beside it is allowed. A
word licenses half a *unit* where a digit licenses half a *place*, which is why
`every_tolerance_follows_its_declared_rule` fires on the second of them — a check nothing in
this slice touched.

**Three of my predictions were wrong, all in the direction of naming a check that did not
fire, and two of the three say something about the machinery rather than about the guess.**

* The backlog tally counts the **list**, not a scan, so no change to a scanner shape can move
  it. That is deliberate — the list and the prose are already matched both ways, so counting
  the list keeps the tally from needing a lesson scan of its own — but it means the tally is
  **not** a second watcher on the detector. Only the ban is.
* Moving a literal with its prose is what stops `every_claim_appears_in_its_own_step` firing,
  which is the whole design of that case: with the sentence and every literal quoting it in
  agreement, the word accounting is the only thing left that can object. It objected.

**And four cases failed setup on the first round**, on anchor strings that had been reflowed
by `cargo fmt` or that matched twice — the ellipsis shape's anchor appears in both scanners,
which is a fact about the two being genuine mirrors. A setup failure is not a green: the
harness reports it as its own outcome rather than counting the case as passed.

## What is not done

* **The other 35** — 48 phrases as the ban counts them — are named in `[[english]]` and
  nothing accounts for any of them. `and-it-is-still-in-there` is the cheapest: eight phrases,
  every one an hour or a fraction of one, and every instant in its reading list is an instant a
  claim on that step is already read at.
* **The reader is still wired in and reads nothing.** Taking `spelled_numbers` out means
  `Written` loses its `scale` and `unit`, which makes `unit_is_time` constant-true and its gate
  dead — the "predicate had a test and the plumbing into it did not" defect one layer over. It
  also collapses the `Reading` bundle, two guards, the `spelled`/`word_blind` partition and a
  dozen paragraphs of prose that assert the current state. That is a slice, not a tail.
* **Titles are not scanned.** `lesson_text` starts at `prose:`, so a lesson's `title` is outside
  both the ledger and the ban — `"The same cell, sixty times the current"` still says it in
  words while the sentence below says `60 times`. Declared here rather than left to be noticed.
* **Two shapes are refused by nothing**, on the terms above: the prose counting its own
  furniture, and a count of engine steps.
